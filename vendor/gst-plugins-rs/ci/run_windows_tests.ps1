$env:ErrorActionPreference='Stop'

[string[]] $exclude_crates = @(
    "--exclude",
    "gst-plugin-csound",
    "--exclude",
    "gst-plugin-webp"
)

[string[]] $features_matrix = @(
    "--no-default-features",
    "",
    "--all-features"
)

if ($env:FDO_CI_CONCURRENT)
{
    $ncpus = $env:FDO_CI_CONCURRENT
}
else
{
    $ncpus = (Get-WmiObject -Class Win32_ComputerSystem).NumberOfLogicalProcessors
}
Write-Host "Build Jobs: $ncpus"
$cargo_opts = @("--color=always", "--jobs=$ncpus", "--all-targets")
$cargo_nextest_opts=@("--profile=ci", "--no-fail-fast", "--no-tests=pass")

function Run-Tests {
    param (
        $Features
    )
    $local_exclude = $exclude_crates;

    # In this case the plugin will pull x11/wayland features
    # which will fail to build on windows.
    if (($Features -eq '--all-features') -or ($Features -eq '')) {
        $local_exclude += @("--exclude", "gst-plugin-gtk4")
    }

    Write-Host "Features: $Features"
    Write-Host "Exclude string: $local_exclude"

    cargo build $cargo_opts --workspace $local_exclude $Features

    if (!$?) {
        Write-Host "Build failed"
        Exit 1
    }

    $env:G_DEBUG="fatal_warnings"
    $env:RUST_BACKTRACE="1"
    cargo nextest run $cargo_opts $cargo_nextest_opts --workspace $local_exclude $Features
    if (!$?) {
        Write-Host "Tests failed"
        Exit 1
    }

    Move-Junit -Features $Features
}

function Move-Junit {
    param (
        $Features
    )

    if ($env:CI_PROJECT_DIR) {
        $parent = $env:CI_PROJECT_DIR
    } else {
        $parent = $PWD.path
    }
    Write-Host "Parent directory: $parent"

    $new_report_dir = "$parent/junit_reports/"
    If(!(test-path -PathType container $new_report_dir))
    {
        New-Item -Path "$new_report_dir" -ItemType "directory"
        if (!$?) {
            Write-Host "Failed to create directory: $new_report_dir"
            Exit 1
        }
    }

    if ($Features -eq "--all-features") {
        $suffix = "all"
    } elseif ($Features -eq "--no-default-features") {
        $suffix = "no-default"
    } else {
        $suffix = "default"
    }

    Move-Item "$parent/target/nextest/ci/junit.xml" "$new_report_dir/junit-$suffix.xml"
    if (!$?) {
        Write-Host "Failed to move junit file"
        Exit 1
    }
}

foreach($feature in $features_matrix) {
    Run-Tests -Features $feature
}
