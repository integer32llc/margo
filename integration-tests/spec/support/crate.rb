# frozen_string_literal: true

class Crate
  attr_reader :name, :version

  def initialize(name:, version:, root:)
    @name = name
    @version = version
    @root = root
    @source = {}
  end

  def lib_rs(content)
    @source['lib.rs'] = content
    self
  end

  def publish_to(registry)
    Dir.mkdir(@root)

    cargo_toml = @root.join('Cargo.toml')
    File.open(cargo_toml, 'w') do |f|
      content = <<~TOML
        [package]
        name = "#{@name}"
        version = "#{@version}"
        edition = "2021"
      TOML
      f.write(content)
    end

    src = @root.join('src')
    Dir.mkdir(src)

    @source.each do |name, content|
      file = src.join(name)
      File.write(file, content)
    end

    system('cargo', 'package', '--quiet', chdir: @root, exception: true)
    package = @root.join('target', 'package', "#{name}-#{version}.crate")

    system(
      '../target/debug/margo',
      'add',
      '--registry',
      registry.root.to_s,
      package.to_s,
      %i[out err] => File::NULL,
      exception: true,
    )
  end
end
