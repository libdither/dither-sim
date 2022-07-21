{
	inputs = {
		nixCargoIntegration.url = "github:yusdacra/nix-cargo-integration";
	};
	outputs = inputs: inputs.nixCargoIntegration.lib.makeOutputs {
		root = ./.;
		overrides = {
			shell = common: prev: with common.pkgs; {
				# hardeningDisable = [ "fortify" ];
				env = prev.env ++ [
					{
						# Vulkan doesn't work without this for some reason :(
						name = "LD_LIBRARY_PATH";
						eval = "$LD_LIBRARY_PATH${pkgs.vulkan-loader}/lib";
					}
					{
						name = "RUSTFLAGS";
						value =
							if common.pkgs.stdenv.isLinux
							then "-C link-arg=-fuse-ld=mold -C target-cpu=native -Clink-arg=-Wl,--no-rosegment"
							else "";
					}
				];
			};
		};
	};
}