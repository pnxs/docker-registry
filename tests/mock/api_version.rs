use tokio::runtime::Runtime;

static API_VERSION_K: &str = "Docker-Distribution-API-Version";
static API_VERSION_V: &str = "registry/2.0";

#[test]
fn test_version_check_status_ok() {
  let mut server = mockito::Server::new();
  let addr = server.url();

  let _m = server
    .mock("GET", "/v2/")
    .with_status(200)
    .with_header(API_VERSION_K, API_VERSION_V)
    .create();

  let runtime = Runtime::new().unwrap();
  let dclient = dockreg::v2::Client::configure()
    .registry(&addr)
    .insecure_registry(true)
    .username(None)
    .password(None)
    .build()
    .unwrap();

  let is_v2 = dclient.is_v2_supported();
  let ok = runtime.block_on(is_v2).unwrap();
  assert!(ok);

  let ensure_v2 = dclient.ensure_v2_registry();
  let _dclient = runtime.block_on(ensure_v2).unwrap();
}

#[test]
fn test_version_check_status_unauth() {
  let mut server = mockito::Server::new();
  let addr = server.url();

  let _m = server
    .mock("GET", "/v2/")
    .with_status(401)
    .with_header(API_VERSION_K, API_VERSION_V)
    .create();

  let runtime = Runtime::new().unwrap();
  let dclient = dockreg::v2::Client::configure()
    .registry(&addr)
    .insecure_registry(true)
    .username(None)
    .password(None)
    .build()
    .unwrap();

  let futcheck = dclient.is_v2_supported();

  let res = runtime.block_on(futcheck).unwrap();
  assert!(res);
}

#[test]
fn test_version_check_status_notfound() {
  let mut server = mockito::Server::new();
  let addr = server.url();

  let _m = server
    .mock("GET", "/v2/")
    .with_status(404)
    .with_header(API_VERSION_K, API_VERSION_V)
    .create();

  let runtime = Runtime::new().unwrap();
  let dclient = dockreg::v2::Client::configure()
    .registry(&addr)
    .insecure_registry(true)
    .username(None)
    .password(None)
    .build()
    .unwrap();

  let futcheck = dclient.is_v2_supported();

  let res = runtime.block_on(futcheck).unwrap();
  assert!(!res);
}

#[test]
fn test_version_check_status_forbidden() {
  let mut server = mockito::Server::new();
  let addr = server.url();

  let _m = server
    .mock("GET", "/v2/")
    .with_status(403)
    .with_header(API_VERSION_K, API_VERSION_V)
    .create();

  let runtime = Runtime::new().unwrap();
  let dclient = dockreg::v2::Client::configure()
    .registry(&addr)
    .insecure_registry(true)
    .username(None)
    .password(None)
    .build()
    .unwrap();

  let futcheck = dclient.is_v2_supported();

  let res = runtime.block_on(futcheck).unwrap();
  assert!(!res);
}

#[test]
fn test_version_check_noheader() {
  let mut server = mockito::Server::new();
  let addr = server.url();

  let _m = server.mock("GET", "/v2/").with_status(403).create();

  let runtime = Runtime::new().unwrap();
  let dclient = dockreg::v2::Client::configure()
    .registry(&addr)
    .insecure_registry(true)
    .username(None)
    .password(None)
    .build()
    .unwrap();

  let futcheck = dclient.is_v2_supported();

  let res = runtime.block_on(futcheck).unwrap();
  assert!(!res);
}

#[test]
fn test_version_check_trailing_slash() {
  let mut server = mockito::Server::new();
  let addr = server.url();

  let _m = server
    .mock("GET", "/v2")
    .with_status(200)
    .with_header(API_VERSION_K, API_VERSION_V)
    .create();

  let runtime = Runtime::new().unwrap();
  let dclient = dockreg::v2::Client::configure()
    .registry(&addr)
    .insecure_registry(true)
    .username(None)
    .password(None)
    .build()
    .unwrap();

  let futcheck = dclient.is_v2_supported();

  let res = runtime.block_on(futcheck).unwrap();
  assert!(!res);
}
