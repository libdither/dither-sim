{
  	inputs = {
		nixpkgs.url = "nixpkgs/nixpkgs-unstable";
		rust-overlay.url = "github:oxalica/rust-overlay";
		utils.url = "github:numtide/flake-utils";
  	};

  	outputs = { self, nixpkgs, utils, rust-overlay, ... }:
	utils.lib.eachDefaultSystem (system: let
		overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
		chan = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
			extensions = [ "rust-src" "clippy" ];
		});
	in rec {
		# `nix develop`
		devShell = pkgs.mkShell {
			buildInputs = with pkgs; [
				chan
				cmake
				pkgconfig
				stdenv.cc.cc.lib
				lld
				
				x11
				xorg.libXcursor
				xorg.libXrandr
				xorg.libXi
				libxkbcommon
				vulkan-tools
				vulkan-headers
				vulkan-loader
				vulkan-validation-layers
				fontconfig
				freetype
			];
			#hardeningDisable = [ "fortify" ];
			#NIX_CFLAGS_LINK = "-fuse-ld=lld";
		};
	});
}