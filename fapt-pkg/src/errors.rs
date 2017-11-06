error_chain!{
    foreign_links {
        Io(::std::io::Error);
        GpgMe(::gpgme::Error);
        Reqwest(::reqwest::Error);
        ReqwestUrl(::reqwest::UrlError);
    }
}
