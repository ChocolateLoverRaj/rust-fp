{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs
  }: let
    pkgs = import nixpkgs {
      system = "x86_64-linux";
    };
    fhs = pkgs.buildFHSUserEnv {
      name = "fhs-shell";
      targetPkgs = pkgs: with pkgs; [
        rustup
	clang
	libclang
	pam
      ];
    };
  in {
    devShells.${pkgs.system}.default = fhs.env;
  };
}
