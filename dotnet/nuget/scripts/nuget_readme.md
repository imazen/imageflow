# Imageflow NuGet Packages

Author notes:

Imageflow.NativeRuntime.win-x86_64 however uses RID win-x64, Imageflow.NativeRuntime.osx-x86_64 uses RID osx-x64, and Imageflow.NativeRuntime.ubuntu-x86_64 uses linux-x64 (it's a duplicate package intentially for backwards compatibility with existing references). We need to not break/change these existing nuget package names

This directory contains the infrastructure for building and packing the various Imageflow NuGet packages using modern .NET SDK tooling (`.csproj`, `.targets`, `dotnet pack`).

## Packages

Several types of packages are generated:

*   **`Imageflow.NativeRuntime.{RID}`**: Contains the native runtime library (`imageflow.dll`, `libimageflow.so`, `libimageflow.dylib`) for a specific .NET [Runtime Identifier](https://docs.microsoft.com/en-us/dotnet/core/rid-catalog) (RID), such as `win-x64`, `linux-arm64`, `osx-x64`.
*   **`Imageflow.NativeTool.{RID}`**: Contains the native command-line tool (`imageflow_tool.exe` or `imageflow_tool`) for a specific RID. Packaged as a .NET Tool.
*   **`Imageflow.NativeRuntime.All`**: A meta-package that references all `Imageflow.NativeRuntime.{RID}` packages.
*   **`Imageflow.Net.All`**: A meta-package that references `Imageflow.Net` (the managed wrapper, published separately) and `Imageflow.NativeRuntime.All`. This is the simplest way for most applications to consume Imageflow.Net with all supported native backends.
*   *Other meta-packages* (e.g., `Imageflow.NativeRuntime.All.Windows`, `Imageflow.Net.All.x64`) may exist to provide more granular dependency sets.

## Compatibility

*   **`Imageflow.NativeRuntime.*` and `Imageflow.NativeTool.*`**: These packages target `netstandard2.0` and `net8.0` respectively for maximum compatibility but contain *native code*. The native code itself has platform and architecture requirements (e.g., `win-x64` requires a 64-bit Windows OS). The included `.targets` files for Windows packages assist with copying the correct native binary (`x86` or `x64`) for `.NET Framework 4.6.1+` projects.
*   **`Imageflow.Net` (External Package)**: Assumed to target `netstandard2.0` for broad compatibility.
*   **Meta-Packages (`*.All`, etc.)**: Target `netstandard2.0` to allow referencing by the widest range of project types, including:
    *   .NET Framework 4.6.1 and later
    *   .NET Core 2.0 and later
    *   .NET 5, 6, 7, 8 and later
    *   Mono, Xamarin, etc.
    
Note that while the meta-package can be referenced, the underlying application host must still be running on a platform/architecture combination for which a corresponding `Imageflow.NativeRuntime.{RID}` package exists and was restored.

## Directory Structure & Mechanism

*(Content moved from previous dotnet/README.md - describes shared/, native/, meta/, test/, Imageflow.sln, and the build/pack process using NativeArtifactBasePath)*

- **`shared/`**: Contains common MSBuild targets (`Imageflow.Common.targets`) and shared assets (README.md, LICENSE.md, icon.png) included in all packages.
- **`native/`**: Contains individual `.csproj` files for each native runtime/tool package per RID. Also contains common targets specific to native runtime (`Imageflow.NativeRuntime.Common.targets`) or native tool (`Imageflow.NativeTool.Common.targets`) packages, and RID-specific `.targets` files (e.g., `targets/Imageflow.NativeRuntime.win-x64.targets`) for native copy logic.
- **`meta/`**: Contains `.csproj` files for meta-packages.
- **`test/`**: Contains a test project (`Imageflow.EndToEnd.Test.csproj`).
- **`Imageflow.sln`**: A solution file including all projects.

## Building and Packing

1.  Build Native Artifacts.
2.  Collect Artifacts into `<staging_dir>/<RID>/native/<binary_filename>`.
3.  Pack Solution: `dotnet pack dotnet/nuget/Imageflow.sln -c Release /p:Version=... /p:ImageflowNetVersion=... /p:NativeArtifactBasePath=<staging_dir> /p:RepositoryUrl=...`

## Testing

`dotnet build dotnet/nuget/test/Imageflow.EndToEnd.Test.csproj -c Release -r <RID> /p:ImageflowNetVersion=...`

## Shared Assets

Ensure `LICENSE.md`, `README.md`, and `icon.png` exist in `dotnet/nuget/shared/`. 

## PowerShell Build & Test Scripts

This section details the PowerShell scripts used in the CI workflow (`.github/workflows/ci.yml`) and for local testing to manage the packaging, testing, and publishing process.

**1. `dotnet/nuget/scripts/read_latest_ver.ps1`**

*   **What it does:** Contains a function `Get-LatestImageflowNetVersion` that queries nuget.org to find the latest published version (including pre-releases) of the `Imageflow.Net` package.
*   **Why:** Used to determine the version constraint for the `Imageflow.Net` dependency when packing the native runtime and tool packages. This ensures the native packages reference a valid, existing `Imageflow.Net` version. It's not currently used in the CI flow (`IMAGEFLOW_NET_VERSION` is hardcoded), but could be used for dynamic versioning.
*   **Contracts:**
    *   **Inputs:** None (function call). Requires internet access to nuget.org and the `NuGet` package provider to be installed.
    *   **Outputs:** Returns the latest version string (e.g., "0.13.2") or `$null` on failure. Prints status messages to the console.
    *   **Assumptions:** `Imageflow.Net` package exists on nuget.org. Network connectivity is available.

**2. `dotnet/nuget/scripts/create_native_placeholders.ps1`**

*   **What it does:** Creates a directory structure containing empty placeholder files that mimic the expected native binary outputs (`.dll`, `.so`, `.dylib`, `.exe`, `imageflow_tool`) for all supported Runtime Identifiers (RIDs).
*   **Why:** Used primarily for local testing (`test-dotnet-workflows.ps1`). It allows testing the packing and testing scripts (`single-pack-and-test.ps1`, `merge-pack-test-publish.ps1`) without needing to perform actual cross-compilation builds, ensuring the directory structure and file finding logic within those scripts works correctly.
*   **Contracts:**
    *   **Inputs:**
        *   `StagingDirectory` (Parameter): Mandatory path where the placeholder structure (e.g., `<StagingDirectory>/win-x64/native/imageflow.dll`) will be created.
    *   **Outputs:** Creates directories and empty files within the specified `StagingDirectory`. Prints status messages.
    *   **Assumptions:** The script knows the correct binary names (`imageflow.dll`, `libimageflow.so`, `libimageflow.dylib`, `imageflow_tool`, `imageflow_tool.exe`) for each platform (Windows, Linux, macOS).

**3. `dotnet/nuget/scripts/single-pack-and-test.ps1`**

*   **What it does:**
    1.  Packs the RID-specific `Imageflow.NativeRuntime.<RID>.csproj` package. It finds the actual native binaries (built in the `build` job) from the provided `NativeArtifactBasePath`.
    2.  Restores, builds, and runs the `Imageflow.EndToEnd.Test.csproj` console application. This test uses the *just-packed* RID-specific native runtime package from the local output directory to ensure the basic packaging and native interop works for that specific RID *before* the final merge.
*   **Why:** Provides an early test for each individual RID's native package *before* the main merge step. This helps catch RID-specific packaging issues (e.g., wrong file paths in the `.csproj`, missing binaries) quickly within the corresponding build job. It prevents waiting until the final `pack-and-publish-nuget` job to discover a problem with a single RID's package. It's run at the end of each `build` job matrix entry in the `ci.yml`.
*   **Contracts:**
    *   **Inputs:**
        *   `RID` (Parameter): Optional. The Runtime Identifier (e.g., `win-x64`). Defaults to the host machine's RID if not provided.
        *   `PackageVersion` (Parameter): Mandatory. The version string for the NuGet package being created.
        *   `ImageflowNetVersion` (Parameter): Mandatory. The version constraint for the `Imageflow.Net` dependency.
        *   `NativeArtifactBasePath` (Parameter): Mandatory. Path to the *directory containing the specific RID's subdirectory* which holds the actual compiled native binaries (e.g., `/path/to/artifacts/native_binaries/` which contains `win-x64/native/*`, `linux-x64/native/*`, etc.). The script expects `<NativeArtifactBasePath>/<RID>/native/` to contain the binaries.
        *   `PackOutputDirectory` (Parameter): Mandatory. Path where the created `.nupkg` file will be placed.
        *   `Configuration` (Parameter): Optional. Build configuration (defaults to `Release`).
    *   **Outputs:**
        *   Creates a `.nupkg` file for `Imageflow.NativeRuntime.<RID>` in `PackOutputDirectory`.
        *   Builds and runs the test application.
        *   Prints status messages. Exits with non-zero code on failure.
    *   **Assumptions:**
        *   `.NET SDK` is installed and available.
        *   The `Imageflow.NativeRuntime.<RID>.csproj` file exists for the specified RID (unless it's a static build like MUSL, where it skips packing).
        *   The `Imageflow.EndToEnd.Test.csproj` exists.
        *   The actual compiled native binaries exist at `<NativeArtifactBasePath>/<RID>/native/`.

**4. `dotnet/nuget/scripts/merge-pack-test-publish.ps1`**

*   **What it does:** This is the main script run in the final `pack-and-publish-nuget` job.
    1.  **Packs Solution:** Cleans, restores, and packs the entire `Imageflow.sln`. This builds the core `Imageflow.Net.csproj` and re-builds/packs the `Imageflow.NativeRuntime.*` projects, this time relying on the MSBuild logic within the `.csproj` files to find the native binaries for *all* RIDs from the `CombinedNativeArtifactBasePath`. It also packs any other projects in the solution (like `Imageflow.AllPlatforms`).
    2.  **Final Test:** Restores, builds, and runs the `Imageflow.EndToEnd.Test.csproj` one last time, using the *final* packages created in step 1 (from the specified `TestRid`). This verifies integration with the final `Imageflow.Net` and `Imageflow.AllPlatforms` packages.
    3.  **Publish:** Conditionally pushes all generated `.nupkg` files to NuGet.org and/or GitHub Packages based on input switches and API keys. Includes logic to delete already pushed packages from a feed if a later push to the same or different feed fails (`DeleteOnFailure`).
*   **Why:** This script aggregates the results from all individual `build` jobs. It performs the final pack process using the combined set of native binaries downloaded as artifacts. It runs a final integration test and handles the conditional publishing logic based on whether it's a release event.
*   **Contracts:**
    *   **Inputs:**
        *   `PackageVersion` (Parameter): Mandatory. Version for all packages.
        *   `ImageflowNetVersion` (Parameter): Optional. Version constraint for `Imageflow.Net` dependency (defaults to `*-*`).
        *   `CombinedNativeArtifactBasePath` (Parameter): Mandatory. Path to the directory containing *all* downloaded RID-specific native binary artifacts (e.g., `/path/to/artifacts/native_binaries/` which contains `win-x64/`, `linux-x64/`, etc., each with a `native/` subfolder).
        *   `NuGetApiKey` (Parameter): Optional. Required if `PushToNuGet` is true.
        *   `NuGetSourceUrl` (Parameter): Optional. Defaults to nuget.org.
        *   `GitHubApiKey` (Parameter): Optional. Required if `PushToGitHub` is true (usually `secrets.GITHUB_TOKEN`).
        *   `GitHubSourceUrl` (Parameter): Optional. Required if `PushToGitHub` is true (e.g., `https://nuget.pkg.github.com/OWNER`).
        *   `PushToNuGet` (Switch): Optional. If present, push to NuGet.org.
        *   `PushToGitHub` (Switch): Optional. If present, push to GitHub Packages.
        *   `DeleteOnFailure` (Switch): Optional. If present (default), attempt to delete pushed packages if any push fails.
        *   `Configuration` (Parameter): Optional. Build configuration (defaults to `Release`).
        *   `TestRid` (Parameter): Optional. RID to use for the final test run. Defaults to host RID or `linux-x64`.
    *   **Outputs:**
        *   Creates multiple `.nupkg` files (e.g., `Imageflow.Net`, `Imageflow.AllPlatforms`, `Imageflow.NativeRuntime.*`) in `artifacts/nuget` within the workspace root.
        *   Builds and runs the test application for `TestRid`.
        *   Conditionally pushes packages to NuGet/GitHub.
        *   Prints status messages. Exits with non-zero code on failure.
    *   **Assumptions:**
        *   `.NET SDK` is installed.
        *   `Imageflow.sln` exists.
        *   `Imageflow.EndToEnd.Test.csproj` exists.
        *   All required native binaries for all RIDs exist under `CombinedNativeArtifactBasePath/<RID>/native/`.
        *   Network connectivity for restore/push.
        *   Valid API keys/URLs if pushing.

**5. `dotnet/nuget/scripts/test-dotnet-workflows.ps1`**

*   **What it does:** A local testing utility script. It orchestrates calls to `create_native_placeholders.ps1`, `single-pack-and-test.ps1`, and `merge-pack-test-publish.ps1` using the generated *placeholder* artifacts.
*   **Why:** Allows developers to quickly test the *logic* of the packing, testing, and merging scripts locally without needing to run the full CI build or have actual cross-compiled binaries available. It verifies parameter passing, directory structures, and basic script execution flow.
*   **Contracts:**
    *   **Inputs:**
        *   `PackageVersion` (Parameter): Optional. Version string to use for the test run (defaults to `0.0.1-localtest`).
    *   **Outputs:** Runs the other scripts. Creates and cleans up temporary directories (`temp_placeholder_staging_for_test`, `temp_single_pack_output`). Prints status messages. Exits non-zero on failure.
    *   **Assumptions:** The other PowerShell scripts (`create_native_placeholders.ps1`, `single-pack-and-test.ps1`, `merge-pack-test-publish.ps1`) exist in the **same directory (`dotnet/nuget/scripts/`)**. `.NET SDK` is installed.
