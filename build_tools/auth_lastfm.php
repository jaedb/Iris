<?php

/**
 * LastFM Authentication proxy
 *
 * To use:
 * 1. Create a LastFM App (https://www.last.fm/api/account/create), and paste in your credentials below
 * 2. Save this script to a publicly-accessible server
 * 3. Set config in mopidy.conf to point to this script
 **/

// LastFM app credentials
define('API_URL','http://ws.audioscrobbler.com/2.0/?format=json');
define('API_KEY','YOUR_KEY_HERE');
define('API_SECRET','YOUR_SECRET_HERE');
define('REDIRECT_URI','YOUR_REDIRECT_URI_HERE');

// Allow cross-domain requests
header("Access-Control-Allow-Origin: *");

// Set our cookies
if (isset($_GET['app'])){
	setcookie('mopidy_iris', $_GET['app'], time()+3600);
}


/* ================================================================================= INIT ================ */
/* ======================================================================================================= */

if (isset($_GET['action'])){
	switch ($_GET['action']){

		case 'authorize':
			header('Location: http://www.last.fm/api/auth/?api_key='.API_KEY.'&cb='.REDIRECT_URI);
			exit;

		case 'start_session':
			$session = startSession($_GET['token']);
			
		    // Pass our error back to the popup opener
			?>	
				<script type="text/javascript">
					window.opener.postMessage( '<?php echo json_encode($session) ?>', "*");
					window.close();
				</script>
			<?php

			break;

		case 'sign_request':			
			$data = $_GET;
			$signed = signRequest($data);
			echo json_encode($signed);
			exit;

		default:
			echo 'Invalid action specified';
			die();

	}
} else {
	echo "No action specified";
	die();
}





/* ================================================================================= GETTERS ============= */
/* ======================================================================================================= */

/**
 * Start a session
 *
 * @param $data = array
 * @param $post = boolean (POST request)
 **/
function startSession($token){
	
	$ch = curl_init();

	if (FALSE === $ch){
		throw new Exception('Failed to initialize');
	}

	$data = signRequest(
		array(
			"method" => "auth.getSession",
			"token" => $token
		), 
		false
	);
	
	curl_setopt($ch, CURLOPT_URL,API_URL);
	curl_setopt($ch, CURLOPT_POST, 1);
	curl_setopt($ch, CURLOPT_SSL_VERIFYHOST, false);
	curl_setopt($ch, CURLOPT_SSL_VERIFYPEER, false);
	curl_setopt($ch, CURLOPT_POSTFIELDS, http_build_query($data));
	curl_setopt($ch, CURLINFO_HEADER_OUT, true);
	curl_setopt($ch, CURLOPT_VERBOSE, true);
	curl_setopt($ch, CURLOPT_FOLLOWLOCATION, true);
	curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
	
	$response = curl_exec($ch);
	
	if (curl_errno($ch)){
		echo 'CURL Error: '. curl_error($ch);
	}
	curl_close($ch);

	$response = json_decode($response, true);

	// Append our other important values to successful signings 
	if (!$response['error']){
		$response['api_key'] = API_KEY;
	}

	return $response;
}


/**
 * Perform a signed request
 *
 * @param $data = array
 * @param $post = boolean (POST request)
 **/
function signRequest($data = array(), $post = true){
	unset($data['action']);
	
	$ch = curl_init();

	if (FALSE === $ch){
		throw new Exception('Failed to initialize');
	}
	
	// Make sure we've got our API key included
	$request = array_merge(array("api_key" => API_KEY), $data);

	// Loop all the values in our request and add to our signature
	$signature = "";
	ksort($request);
	foreach ($request as $key => $value){
		$signature.= $key.$value;
	}

	// Finalize the signature
	$signature.= API_SECRET;
	$signature = md5($signature);
	$request["api_sig"] = $signature;

	return $request;
}
