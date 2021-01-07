
/**
 * Iris builder
 *
 * Collates all static elements into the mopidy_iris Python package folder
 * We also amend all related files with the latest build version
 *
 * To run, execute npm run dev|build|prod|release
 **/

var fs = require('fs');
var path = require('path');
var rimraf = require('rimraf');
var copydir = require('copy-dir');
var copyfile = require('fs-copy-file');

var version = fs.readFileSync("IRIS_VERSION", "utf8");
version = version.replace(/\r?\n?/g, '').trim();
var build = Math.floor(Date.now() / 1000);
console.log('Building version '+version+' ('+build+')');

if (fs.existsSync('mopidy_iris/static/')){
	rimraf.sync('mopidy_iris/static/');
}
fs.mkdirSync('mopidy_iris/static/');

copyfile('src/service-worker.js', 'mopidy_iris/static/service-worker.js', function(error){
	if (error){
		console.log('Build failed, could not copy service-worker.js', error);
		return false;
	}
	console.log('Copied service-worker.js');
});

copyfile('src/manifest.json', 'mopidy_iris/static/manifest.json', function(error){
	if (error){
		console.log('Build failed, could not copy manifest.json', error);
		return false;
	}
	console.log('Copied manifest.json');
});

var assetsFilter = function(stat, filepath, filename) {
	if (stat === "file" && ['.ai', '.psd'].includes(path.extname(filepath))) {
		return false;
	}
	return true;
};
copydir('src/assets', 'mopidy_iris/static/assets', { filter: assetsFilter }, function(error){
	if (error) {
		console.log('Build failed, could not copy assets', error);
		return false;
	}

	console.log('Copied assets');

	var html_file = "mopidy_iris/static/index.html";
	var html_file_content = fs.readFileSync("src/index.html", "utf8");
	html_file_content = html_file_content.replace("VERSION_HERE", version);
	html_file_content = html_file_content.replace("BUILD_HERE", build);
	fs.writeFileSync(html_file, html_file_content, 'utf8');
	console.log('Setting version in HTML');

	console.log('Setting version in manifest.json');
	var manifest_file = "mopidy_iris/static/manifest.json";
	var manifest_file_content = fs.readFileSync(manifest_file, "utf8");
	manifest_file_content = manifest_file_content.replace(/(?:\"manifest_version\"\:\ \")(?:.*)"/, '"manifest_version": "'+version+'"');
	fs.writeFileSync(manifest_file, manifest_file_content, 'utf8');

	console.log('Setting version in __init__.py');
	var init_file = "mopidy_iris/__init__.py";
	var init_file_content = fs.readFileSync(init_file, "utf8");
	init_file_content = init_file_content.replace(/(?:__version__\ \=\ \")(?:.*)"/, '__version__ = "'+version+'"');
	fs.writeFileSync(init_file, init_file_content, 'utf8');

	console.log('Setting version in setup.cfg');
	var cfg_file = "setup.cfg";
	var cfg_file_content = fs.readFileSync(cfg_file, "utf8");
	cfg_file_content = cfg_file_content.replace(/(?:version\ \=\ )(?:.*)/, "version = "+version);
	fs.writeFileSync(cfg_file, cfg_file_content, 'utf8');

	console.log('Setting version in package.json');
	var package_file = "package.json";
	var package_file_content = fs.readFileSync(package_file, "utf8");
	package_file_content = package_file_content.replace(/(?:\"version\"\:\ \")(?:.*)"/, '"version": "'+version+'"');
	fs.writeFileSync(package_file, package_file_content, 'utf8');

	console.log('Done!');
	return true;
});

return false;
