#![no_main]

use libfuzzer_sys::fuzz_target;
use pptx_raster::fuzzing::decode;

/// Image budget the slide has left: a 16 MiB raster, so the RSS limit measures
/// the decode's envelope, which excludes the raster it outputs.
const PIXELS: u64 = 1 << 22;
/// The thread stack the sandbox's nesting bound is measured against.
const STACK: usize = 512 * 1024;

fuzz_target!(|data: &[u8]| {
    let data = data.to_vec();
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || decode(&data, PIXELS))
        .expect("spawn")
        .join()
        .expect("the decode panicked past its guard");
});
