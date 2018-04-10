error_chain!{
    links {
        Parse(::fapt_parse::Error, ::fapt_parse::ErrorKind);
    }

    foreign_links {
        Io(::std::io::Error);
        GpgMe(::gpgme::Error);
        Reqwest(::reqwest::Error);
        ReqwestUrl(::reqwest::UrlError);
        ParseHexError(::hex::FromHexError);
        ParseIntError(::std::num::ParseIntError);
        ParseSystemTimeError(::std::time::SystemTimeError);
        ParseUtf8Error(::std::string::FromUtf8Error);
        Serde(::serde_json::Error);
    }
}

#[cfg(intellij_type_hinting)]
pub use error_chain_for_dumb_ides::stubs::*;
