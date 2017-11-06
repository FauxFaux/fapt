error_chain!{
    foreign_links {
        Io(::std::io::Error);
    }

    links {
        Pkg(::fapt_pkg::Error, ::fapt_pkg::ErrorKind);
    }
}
