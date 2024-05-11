# frozen_string_literal: true

require 'crate'
require 'registry'

class ScratchSpace
  def initialize
    @root = Pathname.new(Dir.mktmpdir)
    @registry = @root.join('registry')
    @crates = @root.join('crates')

    Dir.mkdir(@registry)
    Dir.mkdir(@crates)
  end

  def cleanup
    FileUtils.rm_r(@root)
  end

  def registry
    Registry.new(@registry)
  end

  def crate(name:, version:)
    Crate.new(name:, version:, root: @crates.join(name))
  end
end
