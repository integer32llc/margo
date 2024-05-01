# frozen_string_literal: true

require 'registry_server'

class Registry
  attr_reader :root

  def initialize(root)
    @root = root
    @server = RegistryServer.new(@root)
  end

  def start
    @server.start
    system(
      '../target/debug/margo',
      'init',
      '--base-url',
      url,
      '--defaults',
      @root.to_s,
      %i[out err] => File::NULL,
      exception: true,
    )
  end

  def stop
    @server.stop
  end

  def url
    @server.url
  end
end
