
var fs = require('fs');
var copydir = require('copy-dir');

var filename, file;
var version = fs.readFileSync("build_tools/VERSION.md", "utf8");
var build = Math.floor(Date.now() / 1000);
console.log('Building version '+version+' ('+build+')');

copydir('src/assets', 'mopidy_iris/static/assets', function(error){

	if (error){
		console.log('Build failed, could not copy assets', error);
		return false;

	} else {
		console.log('Copied assets');

		filename = "mopidy_iris/static/index.html";
		fs.createReadStream("src/index.html").pipe(fs.createWriteStream(filename));
		console.log('Copied HTML');

		filename = "mopidy_iris/static/index.html";
		file = fs.readFileSync(filename, "utf8");
		file = file.replace("VERSION_HERE", version);
		file = file.replace("BUILD_HERE", build);
		fs.writeFileSync(filename, file, 'utf8');
		console.log('Setting version in HTML');

		console.log('Setting version in Python');
		filename = "mopidy_iris/__init__.py";
		file = fs.readFileSync(filename, "utf8");
		file = file.replace(/(?:__version__\ \=\ \')(?:.*)'/, "__version__ = '"+version+"'");
		fs.writeFileSync(filename, file, 'utf8');

		console.log('Setting version in NPM');
		filename = "package.json";
		file = fs.readFileSync(filename, "utf8");
		file = file.replace(/(?:\"version\"\:\ \")(?:.*)"/, '"version": "'+version+'"');
		fs.writeFileSync(filename, file, 'utf8');

		console.log('Done!');
		return true;
	}
});

return false;
