# frozen_string_literal: true

require 'scratch_space'

RSpec.describe 'The CLI crate list' do
  let(:scratch) { ScratchSpace.new }
  let(:registry) { scratch.registry }

  before { registry.start }

  after do
    registry.stop
    scratch.cleanup
  end

  it 'shows the crate and version' do
    data = [['alpha', '1.0.0'], ['alpha', '1.1.1'], ['beta', '2.2.2']]

    data.each do |name, version|
      scratch
        .crate(name:, version:)
        .lib_rs(%(pub const ID: &str = "#{name}--#{version}";))
        .publish_to(registry)
    end

    output = registry.list

    aggregate_failures do
      data.each do |name, version|
        expect(output).to match(/#{Regexp.quote(name)}.*#{Regexp.quote(version)}/)
      end
    end
  end
end
