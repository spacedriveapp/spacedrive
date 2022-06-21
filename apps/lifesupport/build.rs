use capnpc::CompilerCommand;

fn main() {
	CompilerCommand::new()
		.file("service.capnp")
		.run()
		.expect("error generating code from Cap'n Proto schema!");
}
