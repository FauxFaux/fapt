error_chain!{
    foreign_links {
        CapnP(::capnp::Error);
        CapnPSchema(::capnp::NotInSchema);
        Io(::std::io::Error);
    }
}
