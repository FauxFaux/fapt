error_chain!{
    foreign_links {
        Io(::std::io::Error);
        GpgMe(::gpgme::Error);
        Reqwest(::reqwest::Error);
        ReqwestUrl(::reqwest::UrlError);
        ParseHexError(::hex::FromHexError);
        ParseIntError(::std::num::ParseIntError);
        ParseUtf8Error(::std::string::FromUtf8Error);
        Serde(::serde_json::Error);
    }
}
