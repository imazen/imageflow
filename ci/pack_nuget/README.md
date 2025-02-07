# Using these packages

First, [you probably want Imageflow.AllPlatforms](https://www.nuget.org/packages/Imageflow.AllPlatforms/) instead, which depends on [all of the NativeRuntime variants](https://www.nuget.org/packages?q=Imageflow.NativeRuntime). If you truly want to only be compatible with a certain os+arch combo, you can install the specific NativeRuntime package you need, along with [Imageflow.Net](https://www.nuget.org/packages/Imageflow.Net/).


[Imageflow.NET](https://github.com/imazen/imageflow-dotnet) is a .NET API for [Imageflow](https://github.com/imazen/imageflow), the fast image optimization and processing library for web servers. Imageflow focuses on security, quality, and performance - in that order. Imageflow.NET is a .NET 8.0 & .NET Standard 2.0 library, and as such is compatible with .NET 4.6.2+, .NET Core 2.0+, and .NET 5/6/7/8/9.


### On .NET Core 3.x and .NET 5/6/7/8 (or if using PackageReference on .NET 4.x)

```
dotnet add package Imageflow.AllPlatforms
```

### If you're still using packages.config on .NET 4.x (such as for ASP.NET projects), you have to install [Imageflow.NativeRuntime.win-x86_64](https://www.nuget.org/packages/Imageflow.NativeRuntime.win-x86_64/), etc. DIRECTLY inside your final application, since NuGet is terrible and can't handle the transitive dependencies.

```
PM> Install-Package Imageflow.Net
PM> Install-Package Imageflow.NativeRuntime.win-x86 -pre
PM> Install-Package Imageflow.NativeRuntime.win-x86_64 -pre
```

Note: On .NET 4.x you must install the [appropriate NativeRuntime(s)](https://www.nuget.org/packages?q=Imageflow+AND+NativeRuntime) in the project you are deploying - they have to copy imageflow.dll to the output folder. They are not copied transitively. 

Also note: Older versions of Windows may not have the C Runtime 
installed ([Install 32-bit](https://aka.ms/vs/16/release/vc_redist.x86.exe) or [64-bit](https://aka.ms/vs/16/release/vc_redist.x64.exe)). 

### License 

* Imageflow is dual licensed under a commercial license and the AGPLv3.
* Imageflow.NET is tri-licensed under a commercial license, the AGPLv3, and the Apache 2 license.
* Imageflow.NET Server is dual licensed under a commercial license and the AGPLv3.
* We offer commercial licenses at https://imageresizing.net/pricing
* Imageflow.NET's Apache 2 license allows for integration with non-copyleft products, as long as jobs are not actually executed (since the AGPLv3/commercial license is needed when libimageflow is linked at runtime). This can allow end-users to benefit from optional imageflow integration in products. 

# Other variants of this package
[Search all of the NativeRuntime variants on nuget.org](https://www.nuget.org/packages?q=Imageflow.NativeRuntime)
