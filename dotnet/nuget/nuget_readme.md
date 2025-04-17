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
