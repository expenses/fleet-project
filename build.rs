fn main() {
    // We need these for cross compiling to windows while avoiding linking to
    // libstdc++ (which is not likely to be installed)
    {
        println!("cargo:rustc-link-lib=static=stdc++");
        println!("cargo:rustc-link-lib=static=gcc");
    }
}
