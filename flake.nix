{
  	inputs = {
		utils.url = "github:numtide/flake-utils";
		naersk.url = "github:nmattia/naersk";
		fenix.url = "github:nix-community/fenix";
  	};

  	outputs = { self, nixpkgs, utils, naersk, fenix }:
	utils.lib.eachDefaultSystem (system: let
		pkgs = nixpkgs.legacyPackages."${system}";
		# Specify Rust Toolchain
		# Use Stable (Default)
		# naersk-lib = naersk.lib."${system}";
		# Use Nightly (provided by fenix)
		naersk-lib = naersk.lib."${system}".override {
			# Use Fenix to get nightly rust
			inherit (fenix.packages.${system}.minimal) cargo rustc;
		};
	in rec {
		# `nix build`
		packages.dbr-sim = naersk-lib.buildPackage {
			pname = "dbr-sim";
			root = ./.;
			buildInputs = with pkgs; [
				cmake
				pkgconfig
				stdenv.cc.cc.lib
				
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
		};
		defaultPackage = packages.dbr-sim;

		# `nix run`
		apps.dbr-sim = utils.lib.mkApp {
			drv = packages.dbr-sim;
		};
		defaultApp = apps.dbr-sim;

		# `nix develop`
		devShell = pkgs.mkShell {
			buildInputs = packages.dbr-sim.buildInputs ++ [ pkgs.lld ];
			LD_LIBRARY_PATH = "${nixpkgs.lib.makeLibraryPath packages.dbr-sim.buildInputs}";
			hardeningDisable = [ "fortify" ];
			NIX_CFLAGS_LINK = "-fuse-ld=lld";
		};
	});
}