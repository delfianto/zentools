## About
EPP or Energy Performance Preference is a feature exposed by modern AMD CPUs (Zen 2 and newer) through the Collaborative Processor Performance Control (CPPC) interface. In Linux, this is primarily managed by the `amd-pstate` kernel driver, specifically when it's operating in `active` mode.

More details can be found in the [Linux kernel documentation](https://docs.kernel.org/admin-guide/pm/amd-pstate.html).

This utility is basically a wrapper for writing to the `sysfs` filesystem, specifically to `/sys/devices/system/cpu/cpu*/cpufreq/energy_performance_preference`. It is written primarily as my medium for learning Rust (also with some help from ChatGPT), so it may not be as polished as other tools. It is, however, functional and should work on most systems with AMD CPUs that support EPP.

## Disclaimer
This utility is provided "as is" without any warranty. It may turn your system into a potato, creates a rift in the space-time continuum, bring about the end of the world, or cause your cat to stop loving you. I take no responsibility for any issues that may arise from using this tool.

## Usage
```
eppcli [OPTIONS]

Manage AMD Energy Performance Preference (EPP) settings.
      --performance          Set EPP profile to 'performance'.
      --balance-performance  Set EPP profile to 'balance-performance'.
      --balance-power        Set EPP profile to 'balance-power'.
      --power                Set EPP profile to 'power'.
  -p <LEVEL>                 Set EPP profile by level.
                             0=performance, 1=balance-performance,
                             2=balance-power, 3=power
  -s, --show                 Show current EPP values for all CPU cores.
  -h, --help                 Print help
  -V, --version              Print version
```

## AMD EPP Profiles
```
- Performance
  Prioritizes performance above power saving.
  The CPU will try to reach higher clock speeds aggressively.
- Balance Performance
  Aims for a balance but leans towards performance
  This is the default value in many systems.
- Balance Power
  Aims for a balance but leans towards power saving.
  More conservative clock speed increases.
- Power
  Strongly prioritizes power saving.
  Favors lower frequencies, may limit peak performance.
```
