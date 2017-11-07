error_chain!{
    foreign_links {
        CapnP(::capnp::Error);
        CapnPSchema(::capnp::NotInSchema);
        Io(::std::io::Error);
        FromUtf8Error(::std::string::FromUtf8Error);
        ParseBoolError(::std::str::ParseBoolError);
        ParseIntError(::std::num::ParseIntError);
    }
}
