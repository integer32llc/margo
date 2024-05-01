# frozen_string_literal: true

require 'webrick'

class RegistryServer
  def initialize(root)
    @server = WEBrick::HTTPServer.new(
      DocumentRoot: root,
      BindAddress: '127.0.0.1',
      Port: 0,
      Logger: WEBrick::Log.new(IO::NULL),
      AccessLog: [],
    )
  end

  def start
    @thread = Thread.new do
      @server.start
    end
  end

  def stop
    @server.shutdown
    @thread.join
  end

  def url
    "http://#{address}:#{port}/"
  end

  def address
    @server.config[:BindAddress]
  end

  def port
    @server.config[:Port]
  end
end
