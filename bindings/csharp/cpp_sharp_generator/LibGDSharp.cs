using System.IO;
using System.Linq;
using System.Reflection;
using System.Text.RegularExpressions;
using CppSharp;
using CppSharp.AST;
using CppSharp.Generators;
using CppSharp.Parser;
using ClangParser = CppSharp.Parser.ClangParser;
using CppAbi = CppSharp.Parser.AST.CppAbi;

namespace LibGD.CLI
{
    public class LibGDSharp : ILibrary
    {
        private readonly string includeDir;
        private readonly string make;
        private readonly string libraryFile;

        public LibGDSharp(string includeDir, string libraryFile)
        {
            this.includeDir = includeDir;
            this.libraryFile = libraryFile;
        }

        public LibGDSharp(string includeDir, string make, string libraryFile)
        {
            this.includeDir = includeDir;
            this.make = make;
            this.libraryFile = libraryFile;
        }

        public void Preprocess(Driver driver, ASTContext ctx)
        {
            ctx.SetClassAsValueType("Png_tag");
            ctx.SetClassAsValueType("Gif_tag");
            ctx.SetClassAsValueType("WBMP_tag");
            ctx.SetClassAsValueType("Jpeg_tag");
            ctx.SetClassAsValueType("Gd_tag");
            ctx.SetClassAsValueType("Gd2_tag");
            ctx.SetClassAsValueType("Xbm_tag");
        }

        public void Postprocess(Driver driver, ASTContext lib)
        {
        }

        public void Setup(Driver driver)
        {
            string dir = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location);
            if (string.IsNullOrEmpty(this.make))
            {
                var parserOptions = new ParserOptions();
                parserOptions.addLibraryDirs(Path.GetDirectoryName(this.libraryFile));
                parserOptions.FileName = Path.GetFileName(this.libraryFile);
                var parserResult = ClangParser.ParseLibrary(parserOptions);
                if (parserResult.Kind == ParserResultKind.Success)
                {
                    var nativeLibrary = CppSharp.ClangParser.ConvertLibrary(parserResult.Library);
                    driver.Options.TargetTriple = nativeLibrary.ArchType == ArchType.x86 ? "i386-pc-windows" : "amd64-pc-windows";
                }
                driver.Options.addDefines("_XKEYCHECK_H");
                driver.Options.CodeFiles.Add(Path.Combine(dir, "_iobuf_VC++2013.cs"));
            }
            else
            {
                string error;
                string output = ProcessHelper.Run(Path.Combine(Path.GetDirectoryName(this.make), "gcc"), "-v", out error);
                if (string.IsNullOrEmpty(output))
                {
                    output = error;
                }
                string target = Regex.Match(output, @"Target:\s*(?<target>[^\r\n]+)").Groups["target"].Value;
                string compilerVersion = Regex.Match(output, @"gcc\s+version\s+(?<version>\S+)").Groups["version"].Value;

                driver.Options.addDefines("_CRTIMP=");
                driver.Options.NoBuiltinIncludes = true;
                driver.Options.MicrosoftMode = false;
                driver.Options.TargetTriple = target;
                driver.Options.Abi = CppAbi.Itanium;

                string gccPath = Path.GetDirectoryName(Path.GetDirectoryName(this.make));
                driver.Options.addSystemIncludeDirs(Path.Combine(gccPath, "include", "c++", compilerVersion));
                driver.Options.addSystemIncludeDirs(Path.Combine(gccPath, "include", "c++", compilerVersion, target));
                driver.Options.addSystemIncludeDirs(Path.Combine(gccPath, target, "include"));
                driver.Options.addSystemIncludeDirs(Path.Combine(gccPath, target, "include", "c++"));
                driver.Options.addSystemIncludeDirs(Path.Combine(gccPath, target, "include", "c++", target));
                driver.Options.addSystemIncludeDirs(Path.Combine(gccPath, "lib", "gcc", target, compilerVersion, "include"));
                driver.Options.CodeFiles.Add(Path.Combine(dir, "_iobuf.cs"));
            }
            driver.Options.addDefines("HAVE_CONFIG_H");
            driver.Options.GeneratorKind = GeneratorKind.CSharp;
            driver.Options.LibraryName = "LibImageFlow";
            driver.Options.OutputNamespace = "ImageFlow";
            driver.Options.Verbose = true;
            driver.Options.IgnoreParseWarnings = true;
            driver.Options.CompileCode = true;
            driver.Options.CheckSymbols = true;
            driver.Options.GenerateDefaultValuesForArguments = true;
            driver.Options.MarshalCharAsManagedChar = true;
            driver.Options.StripLibPrefix = false;
            driver.Options.GenerateSingleCSharpFile = true;
            driver.Options.Headers.AddRange(Directory.EnumerateFiles(this.includeDir, "*.h"));
            driver.Options.addIncludeDirs(includeDir);
            driver.Options.addLibraryDirs(Path.GetDirectoryName(this.libraryFile));
            driver.Options.Libraries.Add(Path.GetFileName(this.libraryFile));
            driver.Options.CodeFiles.Add(Path.Combine(dir, "ImageFlowExtensions.cs"));
        }

        public void SetupPasses(Driver driver)
        {
        }
    }
}
