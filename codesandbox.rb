class Codesandbox < Formula
  desc "Create isolated Ubuntu Docker containers with Claude Code pre-installed"
  homepage "https://github.com/your-username/codesandbox"
  url "https://github.com/your-username/codesandbox/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "YOUR_SHA256_HERE"
  license "MIT"  # Update this based on your actual license

  depends_on "rust" => :build
  depends_on "docker"

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    # Test that the binary was installed and can show help
    assert_match "Code Sandbox - Docker container manager", shell_output("#{bin}/codesandbox --help")
    
    # Test version output
    assert_match version.to_s, shell_output("#{bin}/codesandbox --version")
    
    # Test that it recognizes Docker is not available in test environment
    assert_match "Docker", shell_output("#{bin}/codesandbox 2>&1", 1)
  end
end