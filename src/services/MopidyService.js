
import Mopidy from 'mopidy'

function MopidyService( connection ){
	this.connection = connection;
};

MopidyService.attachKey = 'services.mopidy';

MopidyService.attach = function( app ){
	console.info('MopidyService: Attaching...');

	return new Promise((resolve) => {
		var mopidyhost = 'music.plasticstudio.co';//window.location.hostname;
		var mopidyport = "6680";
		var protocol = 'ws';
		var connection = new Mopidy({
			webSocketUrl: protocol+"://" + mopidyhost + ":" + mopidyport + "/mopidy/ws",
			callingConvention: 'by-position-or-by-name'
		});
		connection.on((a, msg) => {
			//console.log(msg);
			// msg && msg.method ?
			//   console.log('<-', msg.method) :
			//   (msg ? console.log('->', JSON.parse(msg.data)) : console.log('->', msg));
		});
		connection.on('state:online', () => {
			console.info('MopidyService: Attached');
			var service = new MopidyService( connection );
			resolve( service );
		});
	});
}

export default MopidyService;