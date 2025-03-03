# frozen_string_literal: true

require 'capybara/rspec'

$LOAD_PATH << 'spec/support'

RSpec.configure do |config|
  config.expect_with :rspec do |expectations|
    expectations.include_chain_clauses_in_custom_matcher_descriptions = true
  end

  config.mock_with :rspec do |mocks|
    mocks.verify_partial_doubles = true
  end

  config.shared_context_metadata_behavior = :apply_to_host_groups

  config.filter_run_when_matching :focus

  config.example_status_persistence_file_path = 'spec/examples.txt'

  config.disable_monkey_patching!

  config.warnings = true

  config.default_formatter = 'doc' if config.files_to_run.one?

  config.profile_examples = 10

  config.order = :random
  Kernel.srand config.seed

  unless ENV.key?('MARGO_BINARY')
    config.before(:suite) do
      system(
        'cargo', 'build',
        exception: true,
      )
    end
  end
end

MARGO_BINARY = ENV.fetch('MARGO_BINARY', '../target/debug/margo')

Capybara.default_driver = Capybara.javascript_driver = :selenium_headless
Capybara.run_server = false

Capybara::Selenium::Driver.load_selenium
Selenium::WebDriver.logger.ignore(:clear_local_storage, :clear_session_storage)
