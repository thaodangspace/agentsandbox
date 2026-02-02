class Agentsandbox < Formula
  desc "Create isolated Docker containers with AI development agents"
  homepage "https://github.com/thaodangspace/agentsandbox"
  url "https://github.com/thaodangspace/agentsandbox/archive/refs/tags/v0.2.0.tar.gz"
  sha256 "YOUR_SHA256_HERE"
  license "MIT"

  depends_on "go" => :build
  depends_on "docker"

  def install
    system "go", "build", *std_go_args(ldflags: "-s -w"), "./cmd/agentsandbox"
  end

  test do
    # Test that the binary was installed and can show help
    assert_match "Agent Sandbox - Docker container manager", shell_output("#{bin}/agentsandbox --help")
    
    # Test version output
    assert_match version.to_s, shell_output("#{bin}/agentsandbox --version")
    
    # Test that it recognizes Docker is not available in test environment
    output = shell_output("#{bin}/agentsandbox 2>&1", 1)
    assert_match "Docker", output
  end
end
