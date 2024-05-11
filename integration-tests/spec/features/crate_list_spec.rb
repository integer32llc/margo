# frozen_string_literal: true

require 'scratch_space'

RSpec.describe 'The crate list', type: :feature do
  let(:scratch) { ScratchSpace.new }
  let(:registry) { scratch.registry }

  before { registry.start }

  after do
    registry.stop
    scratch.cleanup
  end

  it 'shows the crate and version' do
    names = %w[a bb ccc dddd]

    names.each do |name|
      scratch
        .crate(name:, version: '1.0.0')
        .lib_rs(%(pub const ID: &str = "#{name}";))
        .publish_to(registry)
    end

    visit registry.url

    aggregate_failures do
      names.each do |name|
        within(:section, 'Available crates') do
          expect(page).to have_content(name)
          expect(page).to have_select('version', with_options: ['1.0.0'])
        end
      end
    end
  end

  Capybara.add_selector(:section) do
    xpath { |title| ".//section[h1[contains(., '#{title}')]]" }
  end
end
