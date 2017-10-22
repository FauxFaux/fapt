error_chain!{
    foreign_links {
        CapnP(::capnp::Error);
        Io(::std::io::Error);
    }
}
