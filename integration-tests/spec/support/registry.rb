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
      MARGO_BINARY,
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

  def yank(name:, version:)
    system(
      MARGO_BINARY,
      'yank',
      '--registry',
      @root.to_s,
      name,
      '--version',
      version,
      %i[out err] => File::NULL,
      exception: true,
    )
  end

  def url
    @server.url
  end
end
