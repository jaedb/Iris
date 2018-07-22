
/**
 * Iris builder 
 *
 * Collates all static elements into the mopidy_iris Python package folder
 * We also amend all related files with the latest build version
 *
 * To run, execute npm run dev|build|prod|release
 **/

var fs = require('fs');
var copydir = require('copy-dir');
var copyfiles = require('copy-files');

var version = fs.readFileSync("VERSION.md", "utf8");
version = version.replace(/\r?\n?/g, '').trim();
var build = Math.floor(Date.now() / 1000);
console.log('Building version '+version+' ('+build+')');

copyfiles(
	{
		files: {
			'service-worker.js': 'src/js/service-worker.js',
			'manifest.json': 'src/manifest.json'
		},
		dest: 'mopidy_iris/static/'
	}, function(error){
		if (error){
			console.log('Build failed, could not copy service worker', error);
			return false;
		} else {
			console.log('Copied service worker');
		}
	}
);

copydir('src/assets', 'mopidy_iris/static/assets', function(error){

	if (error){
		console.log('Build failed, could not copy assets', error);
		return false;

	} else {
		console.log('Copied assets');

		var html_file = "mopidy_iris/static/index.html";
		var html_file_content = fs.readFileSync("src/index.html", "utf8");
		html_file_content = html_file_content.replace("VERSION_HERE", version);
		html_file_content = html_file_content.replace("BUILD_HERE", build);
		fs.writeFileSync(html_file, html_file_content, 'utf8');
		console.log('Setting version in HTML');

		console.log('Setting version in Python');
		var init_file = "mopidy_iris/__init__.py";
		var init_file_content = fs.readFileSync(init_file, "utf8");
		init_file_content = init_file_content.replace(/(?:__version__\ \=\ \')(?:.*)'/, "__version__ = '"+version+"'");
		fs.writeFileSync(init_file, init_file_content, 'utf8');

		console.log('Setting version in NPM');
		var package_file = "package.json";
		var package_file_content = fs.readFileSync(package_file, "utf8");
		package_file_content = package_file_content.replace(/(?:\"version\"\:\ \")(?:.*)"/, '"version": "'+version+'"');
		fs.writeFileSync(package_file, package_file_content, 'utf8');

		console.log('Done!');
		return true;
	}
});

return false;
