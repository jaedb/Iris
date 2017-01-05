<?php

// allow cross-domain requests
header("Access-Control-Allow-Origin: *");

$url = 'http://jamesbarnsley.co.nz/auth.php';

if( isset($_GET['app']) )
	setcookie( 'mopidy_iris', $_GET['app'], time()+3600 );



/* ================================================================================= INIT ================ */
/* ======================================================================================================= */

// we've just completed authorization, now create credentials (access_token, etc)
if( isset($_GET['code']) ){
	
	// go get our credentials
	$response = getToken( $_GET['code'], $url );	
	$responseArray = json_decode( $response, true );
	
	// add our code to the array of credentials, etc
	$responseArray['authorization_code'] = $_GET['code'];
	$response = json_encode($responseArray);
	
	// make sure we have a successful response
	if( !isset($responseArray['access_token']) ){
		echo 'Error!';
		die();
	}
	
	// send our data back to Spotmop
	?>	
		<script type="text/javascript">
			window.opener.postMessage( '<?php echo $response ?>', "*");
			window.close();
		</script>
	<?php

// refresh existing token
}else if( isset($_GET['action']) && $_GET['action'] == 'refresh' && $_GET['refresh_token'] ){
	header('Content-Type: application/json');
	$response = refreshToken( $_GET['refresh_token'] );
	
	// parse our response code to our response
	header(' ', true, $response['response_code'] );

	// create the body of our response
	echo $response['exec'];
	die();

// fresh authentication, so let's get one
}else if( isset($_GET['action']) && $_GET['action'] == 'authorize' ){
	getAuthorizationCode( $url );
}





/* ================================================================================= GETTERS ============= */
/* ======================================================================================================= */



/**
 * Acquire an authorization code.
 *
 * This is what connects an account's authorization for this app to use their account 
 * for future tokens and queries. Redirects to Spotify.
 * @param $url = redirect url (this script)
*/
function getAuthorizationCode( $url ){

	$popup = 'https://accounts.spotify.com/authorize?client_id=01d4ca2e9f4f415c80502431a6aa4200&redirect_uri='.$url.'&scope=playlist-modify-private%20playlist-modify-public%20playlist-read-private%20playlist-modify-private%20user-library-read%20user-library-modify%20user-follow-modify%20user-follow-read%20user-top-read&response_type=code&show_dialog=true';
	
	?>
		<script tye="text/javascript">
		
			// open an authentication request window (to spotify)
			var popup = window.open("<?php echo $popup ?>","popup","height=680,width=400");

			// listen for incoming messages from the popup
			window.addEventListener('message', function(event){
				
				// only allow incoming data from our authentication proxy site
				if( !/^https?:\/\/jamesbarnsley\.co\.nz/.test(event.origin) )
					return false;

				// pass the message on to the Angular application
				window.parent.postMessage( event.data, '*' );
			}, false);

			var timer = setInterval(checkPopup, 1000);
            function checkPopup(){
                if( typeof(popup) !== 'undefined' && popup ){
	                if( popup.closed ){
	                    window.parent.postMessage( 'closed', '*' );
	                    clearInterval(timer);
	                }
                }else{
                    window.parent.postMessage( 'blocked', '*' );
                    clearInterval(timer);
                }
            }

		</script>
	<?php
}

/*
 * Get a new access token
 * Creates a request to Spotify, which returns a new access_token, refresh_token and token_expiry object
 * @param $code = string
 * @param $url = redirect url (this script)
*/
function getToken( $code, $url ){
	
	$ch = curl_init();

	if (FALSE === $ch)
		throw new Exception('Failed to initialize');
		
	$post_data = array(
			'client_id' => '01d4ca2e9f4f415c80502431a6aa4200',
			'client_secret' => '7352c9c74791478ab02be0f004ec9541',
			'grant_type' => 'authorization_code',
			'code' => $code,
			'redirect_uri' => $url
		);
	
	curl_setopt($ch, CURLOPT_URL,"https://accounts.spotify.com/api/token");
	curl_setopt($ch, CURLOPT_POST, 1);
	curl_setopt($ch, CURLOPT_SSL_VERIFYHOST, false);
	curl_setopt($ch, CURLOPT_SSL_VERIFYPEER, false);
	curl_setopt($ch, CURLOPT_POSTFIELDS, http_build_query($post_data));
	curl_setopt($ch, CURLINFO_HEADER_OUT, true);
	curl_setopt($ch, CURLOPT_VERBOSE, true);
	curl_setopt($ch, CURLOPT_FOLLOWLOCATION, true);
	curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
	
	$response = curl_exec($ch);
	
	if(curl_errno($ch)){
		echo 'CURL Error: '. curl_error($ch);
	}
	
	curl_close($ch);
	
	return $response;
}




/*
 * Refresh a token 
 * Creates a request to Spotify, which returns a new access_token, refresh_token and token_expiry object
 * @var $refresh_token = string
*/
function refreshToken($refresh_token){
	
	$ch = curl_init();

	$post_data = array(
			'client_id' => '01d4ca2e9f4f415c80502431a6aa4200',
			'client_secret' => '7352c9c74791478ab02be0f004ec9541',
			'grant_type' => 'refresh_token',
			'refresh_token' => $refresh_token
		);

	curl_setopt($ch, CURLOPT_URL,"https://accounts.spotify.com/api/token");
	curl_setopt($ch, CURLOPT_POST, 1);
	curl_setopt($ch, CURLOPT_SSL_VERIFYHOST, false);
	curl_setopt($ch, CURLOPT_SSL_VERIFYPEER, false);
	curl_setopt($ch, CURLOPT_POSTFIELDS, http_build_query($post_data));
	curl_setopt($ch, CURLINFO_HEADER_OUT, true);
	curl_setopt($ch, CURLOPT_VERBOSE, true);
	curl_setopt($ch, CURLOPT_FOLLOWLOCATION, true);
	curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);

	curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
	$exec = curl_exec($ch);
	$info = curl_getinfo($ch);
	$response = array(
		'exec' => $exec,
		'response_code' => $info['http_code']
	);
	curl_close ($ch);
	
	return $response;
}








