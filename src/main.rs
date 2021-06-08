use std::env;
use cfs::Container;

fn main() {
    let args: Vec<String> = env::args().collect();

    let container = Container::new(&args);
    container.run();
}
