### [Imageflow](https://github.com/imazen/imageflow) was designed from the ground up for web servers, to give you the fastest image optimization without compromising security or quality. Imageflow is written in Rust.

### [Imageflow.NET](https://github.com/imazen/imageflow-dotnet) provides an easy-to-use C# API for resizing, cropping, and optimizing images, and is compatible with .NET 5/6/7/8/9, .NET 4.6.2+, and any .NET Standard 2.0 (including Xamarin, etc). 

`dotnet add package Imageflow.Net.All` ([nuget](https://www.nuget.org/packages/Imageflow.Net.All/)) to get started, then [README](https://github.com/imazen/imageflow-dotnet).

## What you want, what you *really, really* want

### [Imageflow Server](https://github.com/imazen/imageflow-server), so you can add `?w=100&format=webp` etc. to your image URLs and end your reponsive image woes. It gives you enterprise-grade image delivery, startup-grade ease of use, speedy disk caching (with a cutting edge WAL database designed for big files), and multi-cloud storage integration. 

### [Seriously, it's not hard to deploy as a microservice](https://github.com/imazen/imageflow-server) and it's a lot better than trying to code all of that yourself, without making a security mistake.

[Here, have some diagrams and marketing](https://imazen.io).

---

### Guide to Imageflow.Net / Imageflow.NativeRuntime packages

This package version matches the Imageflow version ([github](https://github.com/imazen/imageflow)) it was created with. [Imageflow.Net](https://www.nuget.org/packages/Imageflow.Net/) ([github](https://github.com/imazen/imageflow-dotnet)) is versioned/released separately.


## Packages that include Imageflow.Net + imageflow.dll (Imageflow.NativeRuntime.*)

* [Imageflow.Net.All](https://www.nuget.org/packages/Imageflow.Net.All/) = [Imageflow.Net](https://www.nuget.org/packages/Imageflow.Net/) + [Imageflow.NativeRuntime.All](https://www.nuget.org/packages/Imageflow.NativeRuntime.All/). Same as [Imageflow.AllPlatforms](https://www.nuget.org/packages/Imageflow.AllPlatforms/), EXCEPT it is updated every time Imageflow is published, instead of when Imageflow.NET is published.
* [Imageflow.Net.All.x64](https://www.nuget.org/packages/Imageflow.Net.All.x64/) = [Imageflow.Net](https://www.nuget.org/packages/Imageflow.Net/) + [Imageflow.NativeRuntime.All.x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.All.x64/)
* [Imageflow.Net.All.Arm64](https://www.nuget.org/packages/Imageflow.Net.All.Arm64/) = [Imageflow.Net](https://www.nuget.org/packages/Imageflow.Net/) + [Imageflow.NativeRuntime.All.Arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.All.Arm64/)
* [Imageflow.Net.All.Windows](https://www.nuget.org/packages/Imageflow.Net.All.Windows/) = [Imageflow.Net](https://www.nuget.org/packages/Imageflow.Net/) + [Imageflow.NativeRuntime.All.Windows](https://www.nuget.org/packages/Imageflow.NativeRuntime.All.Windows/)
* [Imageflow.Net.All.Linux](https://www.nuget.org/packages/Imageflow.Net.All.Linux/) = [Imageflow.Net](https://www.nuget.org/packages/Imageflow.Net/) + [Imageflow.NativeRuntime.All.Linux](https://www.nuget.org/packages/Imageflow.NativeRuntime.All.Linux/)
* [Imageflow.Net.All.Mac](https://www.nuget.org/packages/Imageflow.Net.All.Mac/) = [Imageflow.Net](https://www.nuget.org/packages/Imageflow.Net/) + [Imageflow.NativeRuntime.All.Mac](https://www.nuget.org/packages/Imageflow.NativeRuntime.All.Mac/)

### The NativeRuntime packages also have meta packages:
* [Imageflow.NativeRuntime.All](https://www.nuget.org/packages/Imageflow.NativeRuntime.All/) = [win-x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.win-x64/) + [win-x86](https://www.nuget.org/packages/Imageflow.NativeRuntime.win-x86/) + [win-arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.win-arm64/) + [linux-x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.linux-x64/) + [linux-arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.linux-arm64/) + [osx-x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.osx-x64/) + [osx-arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.osx-arm64/)

* [Imageflow.NativeRuntime.All.x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.All.x64/) = [win-x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.win-x64/) + [linux-x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.linux-x64/) + [osx-x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.osx-x64/)
* [Imageflow.NativeRuntime.All.Arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.All.Arm64/) = [win-arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.win-arm64/) + [linux-arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.linux-arm64/) + [osx-arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.osx-arm64/)
* [Imageflow.NativeRuntime.All.Windows](https://www.nuget.org/packages/Imageflow.NativeRuntime.All.Windows/) = [win-x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.win-x64/) + [win-x86](https://www.nuget.org/packages/Imageflow.NativeRuntime.win-x86/) + [win-arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.win-arm64/)
* [Imageflow.NativeRuntime.All.Linux](https://www.nuget.org/packages/Imageflow.NativeRuntime.All.Linux/) = [linux-x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.linux-x64/) + [linux-arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.linux-arm64/)
* [Imageflow.NativeRuntime.All.Mac](https://www.nuget.org/packages/Imageflow.NativeRuntime.All.Mac/) = [osx-x64](https://www.nuget.org/packages/Imageflow.NativeRuntime.osx-x64/) + [osx-arm64](https://www.nuget.org/packages/Imageflow.NativeRuntime.osx-arm64/)

*If you only want to target a single os+arch combo, you can install the specific [Imageflow.NativeRuntime.* package](https://www.nuget.org/packages?q=Imageflow.NativeRuntime) you need and [Imageflow.Net](https://www.nuget.org/packages/Imageflow.Net/) separately.*

## .NET 4.x compatibility notes 

On .NET 4.x you must install the [appropriate NativeRuntime(s)](https://www.nuget.org/packages?q=Imageflow+AND+NativeRuntime) in the project you are deploying - they have to copy imageflow.dll to the output folder. They are not copied transitively. 

## Packages.config compatibility notes 

If you're still using packages.config on .NET 4.x (such as for ASP.NET projects), you have to install [Imageflow.NativeRuntime.win-x86_64](https://www.nuget.org/packages/Imageflow.NativeRuntime.win-x86_64/), [etc.](https://www.nuget.org/packages?q=Imageflow.NativeRuntime) DIRECTLY inside your final application, since NuGet is terrible and can't handle the transitive dependencies.

```
PM> Install-Package Imageflow.Net
PM> Install-Package Imageflow.NativeRuntime.win-x86 -pre
PM> Install-Package Imageflow.NativeRuntime.win-x86_64 -pre
```

## Older versions of Windows may not have the C Runtime 

Older versions of Windows may not have the C Runtime 
installed ([Install 32-bit](https://aka.ms/vs/16/release/vc_redist.x86.exe) or [64-bit](https://aka.ms/vs/16/release/vc_redist.x64.exe)). 

# License Terms

* Imageflow is dual licensed under a commercial license and the AGPLv3.
* Imageflow.NET is tri-licensed under a commercial license, the AGPLv3, and the Apache 2 license.
* Imageflow.NET Server is dual licensed under a commercial license and the AGPLv3.
* We offer commercial licenses at https://imageresizing.net/pricing


# Why Imageflow.NET is Apache 2 licensed

Imageflow.NET's Apache 2 license allows for integration with non-copyleft products, as long as jobs are not actually executed (since the AGPLv3/commercial license is needed when libimageflow is linked at runtime). This can allow end-users to benefit from optional imageflow integration in products. 

# Other variants of this package
[Search all of the NativeRuntime variants on nuget.org](https://www.nuget.org/packages?q=Imageflow.NativeRuntime)
