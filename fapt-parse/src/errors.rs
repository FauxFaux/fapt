error_chain!{
    foreign_links {
        Io(::std::io::Error);
        FromUtf8Error(::std::string::FromUtf8Error);
        ParseBoolError(::std::str::ParseBoolError);
        ParseIntError(::std::num::ParseIntError);
    }
}
