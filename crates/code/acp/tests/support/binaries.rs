pub fn acp_binary() -> String {
    std::env::var_os("ASTER_ACP_BIN")
        .map(|path| path.into_string().expect("ASTER_ACP_BIN must be UTF-8"))
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_aster-acp").to_owned())
}
