use detour::static_detour;
use std::error::Error;
use std::{ffi::CString, iter, mem};
use winapi::ctypes::c_int;
use winapi::shared::minwindef::{BOOL, DWORD, HINSTANCE, LPVOID, TRUE};
use winapi::shared::ws2def::SOCKADDR;
use winapi::um::libloaderapi::{GetModuleHandleW, GetProcAddress};
use winapi::um::winnt::DLL_PROCESS_ATTACH;
use winapi::um::winsock2::SOCKET;

static_detour! {
  static ConnectHook: unsafe extern "system" fn(SOCKET, *const SOCKADDR, c_int) -> c_int;
}

type FnConnect = unsafe extern "system" fn(SOCKET, *const SOCKADDR, c_int) -> c_int;

unsafe fn main() -> Result<(), Box<dyn Error>> {
  let address =
    get_module_symbol_address("ws2_32.dll", "connect").expect("could not find 'connect' address");
  let target: FnConnect = mem::transmute(address);

  ConnectHook.initialize(target, connect_detour)?.enable()?;
  Ok(())
}

fn connect_detour(s: SOCKET, name: *const SOCKADDR, namelen: c_int) -> c_int {
  let mut socket_addr = unsafe { *name.as_ref().unwrap() }.clone();

  let mut data = [0; 14];
  for i in 0..14 {
    data[i] = socket_addr.sa_data[i];
  }

  let port = u16::from_be_bytes([data[0] as u8, data[1] as u8]);

  if port == 443 || port == 5555 {
    let new_port = 5555_u16.to_be_bytes();

    data[0] = new_port[0] as i8;
    data[1] = new_port[1] as i8;
    data[2] = 127;
    data[3] = 0;
    data[4] = 0;
    data[5] = 1;

    unsafe {
      socket_addr.sa_data = data;
      ConnectHook.call(s, &socket_addr, namelen)
    }
  } else {
    unsafe { ConnectHook.call(s, name, namelen) }
  }
}

fn get_module_symbol_address(module: &str, symbol: &str) -> Option<usize> {
  let module = module
    .encode_utf16()
    .chain(iter::once(0))
    .collect::<Vec<u16>>();
  let symbol = CString::new(symbol).unwrap();
  unsafe {
    let handle = GetModuleHandleW(module.as_ptr());
    match GetProcAddress(handle, symbol.as_ptr()) as usize {
      0 => None,
      n => Some(n),
    }
  }
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllMain(
  _module: HINSTANCE,
  call_reason: DWORD,
  _reserved: LPVOID,
) -> BOOL {
  if call_reason == DLL_PROCESS_ATTACH {
    main().is_ok() as BOOL
  } else {
    TRUE
  }
}
