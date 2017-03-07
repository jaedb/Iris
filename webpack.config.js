
const isDev = process.env.NODE_ENV !== "production";
const path = require('path');
const webpack = require('webpack');
const ExtractTextPlugin = require('extract-text-webpack-plugin');

const node_dir = path.resolve(__dirname, 'node_modules');
const output_dir = path.resolve(__dirname, 'mopidy_iris/static');

const config = {
	
	context: path.resolve(__dirname),

	entry: {
		js: './src/js/index'
	},
	
	output: {
		path: path.resolve(__dirname, 'mopidy_iris/static'),
		filename: 'app.js'
	},
	
	module: {
		rules: [
			{
				test: require.resolve('jquery'),
        		exclude: [
        			/node_modules/
        		],
				use: 'expose-loader?jQuery!expose?$'
			},
			{
				// JSX
				test: /\.js$/,
				exclude: [
        			/node_modules/,
					'index.js',
				],
				use: {
					loader: 'babel-loader',
					options: {
						presets: [
							'react',
							'es2015',
							'stage-2'
						]
					}
				}
			},
			{
				test: /\.scss$/,
				use: ExtractTextPlugin.extract({
					fallback: 'style-loader',
					//resolve-url-loader may be chained before sass-loader if necessary
					use: ['css-loader', 'sass-loader']
				})
			},
			{
				// load external resources (ie Google fonts)
				test: /.(gif|png|woff(2)?|eot|ttf|svg)(\?[a-z0-9=\.]+)?$/,
				use: { 
					loader: 'url-loader',
					options: {
						'limit': 100000
					}
				}
			}
		]
	},
	
	plugins: [
		new webpack.ProvidePlugin({
		    $: "jquery",
		    jQuery: "jquery",
		    "window.jQuery": "jquery"
		})
	]
};

/**
 * Development-only configuration values
 **/
if (isDev){
	
	// set compiled css location
	config.plugins.push( new ExtractTextPlugin("app.css") );
	
	// we want source maps
	config.devtool = 'source-map';
	
	
/**
 * Production-only configuration values
 **/
} else {
	
	// set our final output filename
	config.output.filename = 'app.min.js';
	
	// re-iterate our production value as a string (for ReactJS building)
	config.plugins.push(
		new webpack.DefinePlugin({
			'process.env':{
				'NODE_ENV': JSON.stringify('production')
			}
		})
	);
	
	// remove all debug and console code
	config.module.rules.push(
		{ 
			test: /\.(js)$/,
			use: {
				loader: 'webpack-strip',
				options: {
					strip: ['console.log', 'console.info', 'debug']
				}
			}
		}
	);
	
	// set compiled css location
	config.plugins.push( new ExtractTextPlugin("app.min.css") );
	
	// uglify our js, with no sourcemaps
	config.plugins.push(
			new webpack.optimize.UglifyJsPlugin({
				compress: true,
				mangle: false,
				sourceMap: false,
				comments: false
			})
		);
}

// now export our collated config object
module.exports = config;
