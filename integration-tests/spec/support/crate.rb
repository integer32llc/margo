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
    dir = @root.join(version)
    FileUtils.mkdir_p(dir)

    cargo_toml = dir.join('Cargo.toml')
    File.open(cargo_toml, 'w') do |f|
      content = <<~TOML
        [package]
        name = "#{@name}"
        version = "#{@version}"
        edition = "2021"
      TOML
      f.write(content)
    end

    src = dir.join('src')
    Dir.mkdir(src)

    @source.each do |name, content|
      file = src.join(name)
      File.write(file, content)
    end

    system('cargo', 'package', '--quiet', chdir: dir, exception: true)
    package = dir.join('target', 'package', "#{name}-#{version}.crate")

    system(
      MARGO_BINARY,
      'add',
      '--registry',
      registry.root.to_s,
      package.to_s,
      %i[out err] => File::NULL,
      exception: true,
    )
  end
end
