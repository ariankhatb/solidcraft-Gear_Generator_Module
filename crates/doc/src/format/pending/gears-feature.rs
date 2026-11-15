//! A design can hold a `gears` feature (Gear Generator): a whole gear set or drivetrain as one
//! timeline feature, `params` (the set's `gear.create` parameters, a JSON object), `sketches`
//! (the sketches it is read from, by feature id), `operation` and `targets` as any tool feature's,
//! and `keep_out_cuts` (bodies its keep-out body is cut from). Its bodies are built in memory each
//! evaluation; no sketches or other features are saved for them. Older designs keep their gear
//! sets as folders of ordinary features with the generator's record in the plugin data, and open
//! as they are: nothing is rewritten. The last release warns about the newer format number, keeps
//! the feature in its place without building it (`crates/doc/src/newer_file.rs`) and saves it
//! back as it came.
