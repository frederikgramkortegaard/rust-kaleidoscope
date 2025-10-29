use std::collections::HashMap;
use std::io::{self, Write};

// Define all available extern functions here

extern "C" fn putchard(x: f64) -> f64 {
    print!("{}", x as u8 as char);
    io::stdout().flush().unwrap();
    0.0
}

extern "C" fn printd(x: f64) -> f64 {
    println!("{}", x);
    0.0
}

pub struct FfiRegistry {
    functions: HashMap<String, usize>,
}

impl FfiRegistry {
    pub fn new() -> Self {
        let mut functions = HashMap::new();

        // Register available extern functions
        functions.insert("putchard".to_string(), putchard as usize);
        functions.insert("printd".to_string(), printd as usize);

        FfiRegistry { functions }
    }

    pub fn get(&self, name: &str) -> Option<usize> {
        self.functions.get(name).copied()
    }
}
