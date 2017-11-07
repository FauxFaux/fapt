extern crate capnpc;

fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("schema")
        .file("../apt.capnp")
        .run()
        .expect("schema compiler command");
}
