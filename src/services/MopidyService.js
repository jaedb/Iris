
import Mopidy from 'mopidy'


/**
 * Create an IOC wrapper for our MopidyService instance
 * 
 * This initiates our service, and provides a container for our connection. We wrap
 * this in a Promise so any requests to .get() will be delayed until we're connected. Genius!
 **/

let MopidyServiceWrapper = {
	attachKey: 'services.mopidy',
	attach: function( store ){
		console.info('MopidyServiceWrapper: Attaching...');
		var service = new MopidyService( store );
		return new Promise((resolve) => {
			service.connection.on('state:online', () => {
				resolve( service );
			});
		});
	}
}

export default MopidyServiceWrapper


/**
 * Mopidy service
 *
 * Handles internal requests and passes them on to our connection
 **/
class MopidyService {

	constructor( store ){
		var mopidyhost = 'music.plasticstudio.co';//window.location.hostname;
		var mopidyport = "6680";
		var protocol = 'ws';

		this.connection = new Mopidy({
			webSocketUrl: protocol+"://" + mopidyhost + ":" + mopidyport + "/mopidy/ws",
			callingConvention: 'by-position-or-by-name'
		});
		this.connection.on(
			(type, message) => this.handleMessage( type, message )
		);
		this.store = store;
	}

	setConnection( connection ){
		this.connection = connection;
	}

	clearConnection(){
		this.connection = false;
	}

	handleMessage( type, message ){
		//console.log( message );
	}

	getCurrentTracklist(){
		this.connection.tracklist.getTlTracks()
			.then( function(tracks){
				console.log(tracks);
			});
	}

}