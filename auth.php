<?php

// Spotify app credentials
// Create your application here: https://developer.spotify.com/my-applications
define('CLIENT_ID','YOUR_ID_HERE');
define('CLIENT_SECRET','YOUR_SECRET_HERE');
define('REDIRECT_URI','YOUR_REDIRECT_URI_HERE (to this file)');

// Allow cross-domain requests
header("Access-Control-Allow-Origin: *");

// Set our cookies
if (isset($_GET['app'])){
	setcookie( 'mopidy_iris', $_GET['app'], time()+3600 );
}


/* ================================================================================= INIT ================ */
/* ======================================================================================================= */

// we've just completed authorization, now create credentials (access_token, etc)
if (isset($_GET['code'])){
	
	// go get our credentials
	$response = getToken($_GET['code']);
	$responseArray = json_decode( $response, true );
	
	// add our code to the array of credentials, etc
	$responseArray['authorization_code'] = $_GET['code'];
	$response = json_encode($responseArray);
	
	// make sure we have a successful response
	if( !isset($responseArray['access_token']) ){
		echo 'Error!';
		die();
	}
	
    // Pass our error back to the popup opener
	?>	
		<script type="text/javascript">
			window.opener.postMessage( '<?php echo $response ?>', "*");
			window.close();
		</script>
	<?php

// authorization error
} else if (isset($_GET['error'])){

        // Pass our error back to the popup opener
        ?>
                <script type="text/javascript">
                        window.opener.postMessage("{\"error\": \"<?php echo $_GET['error'] ?>\"}", "*");
                        window.close();
                </script>
        <?php

// refresh existing token
} else if (isset($_GET['action']) && $_GET['action'] == 'refresh' && $_GET['refresh_token']){
	header('Content-Type: application/json');
	$response = refreshToken( $_GET['refresh_token'] );
	
	// parse our response code to our response
	header(' ', true, $response['response_code'] );

	// create the body of our response
	echo $response['exec'];
	die();

// fresh authentication, so let's get one
} else if (isset($_GET['action']) && $_GET['action'] == 'authorize'){

	// Simply redirect to the authorization panel
	header('Location: https://accounts.spotify.com/authorize?client_id='.CLIENT_ID.'&redirect_uri='.REDIRECT_URI.'&scope='.$_GET['scope'].'&response_type=code&show_dialog=true');

	exit;
}





/* ================================================================================= GETTERS ============= */
/* ======================================================================================================= */


/*
 * Get a new access token
 * Creates a request to Spotify, which returns a new access_token, refresh_token and token_expiry object
 * @param $code = string
*/
function getToken($code){
	
	$ch = curl_init();

	if (FALSE === $ch)
		throw new Exception('Failed to initialize');
		
	$post_data = array(
			'client_id' => CLIENT_ID,
			'client_secret' => CLIENT_SECRET,
			'grant_type' => 'authorization_code',
			'code' => $code,
			'redirect_uri' => REDIRECT_URI
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
			'client_id' => CLIENT_ID,
			'client_secret' => CLIENT_SECRET,
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