<?php

// Spotify app credentials are injected via env vars from a Kubernetes
// Secret (see k8s/auth-server-secret.example.yaml) - never hardcode real
// credentials here, this file is committed to git.
$clientId = getenv('SPOTIFY_CLIENT_ID');
$clientSecret = getenv('SPOTIFY_CLIENT_SECRET');
$redirectUri = getenv('SPOTIFY_REDIRECT_URI');

// Allow cross-domain requests
header("Access-Control-Allow-Origin: *");

// Set our cookies
if (isset($_GET['app'])) {
    setcookie('mopidy_iris', $_GET['app'], time() + 3600);
}


/* ================================================================================= INIT ================ */
/* ======================================================================================================= */

// we've just completed authorization, now create credentials (access_token, etc)
if (isset($_GET['code'])) {

    // go get our credentials
    $response = getToken($_GET['code'], $clientId, $clientSecret, $redirectUri);
    $response = json_decode($response, true);

    // add our code to the array of credentials, etc
    $response['authorization_code'] = $_GET['code'];
    $response['origin'] = "auth_spotify";

    // make sure we have a successful response
    if (!isset($response['access_token'])) {
        echo 'Error!';
        die();
    }

    // Pass our error back to the popup opener
    ?>
        <script type="text/javascript">
            window.opener.postMessage( '<?php echo json_encode($response) ?>', "*");
            window.close();
        </script>
    <?php

// authorization error
} else if (isset($_GET['error'])) {

    $response = array(
        "error" => $_GET["error"],
        "origin" => "auth_spotify"
    );

    // Pass our error back to the popup opener
    ?>
            <script type="text/javascript">
                    window.opener.postMessage('<?php echo json_encode($response) ?>', "*");
                    window.close();
            </script>
    <?php

// refresh existing token
} else if (isset($_GET['action']) && $_GET['action'] == 'refresh' && $_GET['refresh_token']) {
    header('Content-Type: application/json');
    $response = refreshToken($_GET['refresh_token'], $clientId, $clientSecret);

    // parse our response code to our response
    header(' ', true, $response['response_code']);

    // create the body of our response
    echo $response['exec'];
    die();

// fresh authentication, so let's get one
} else if (isset($_GET['action']) && $_GET['action'] == 'authorize') {

    // Simply redirect to the authorization panel
    header('Location: https://accounts.spotify.com/authorize?client_id=' . $clientId . '&redirect_uri=' . $redirectUri . '&scope=' . $_GET['scope'] . '&response_type=code&show_dialog=true');

    exit;
}


/* ================================================================================= GETTERS ============= */
/* ======================================================================================================= */


/*
 * Get a new access token
 * Creates a request to Spotify, which returns a new access_token, refresh_token and token_expiry object
 */
function getToken($code, $clientId, $clientSecret, $redirectUri)
{

    $ch = curl_init();

    if (FALSE === $ch)
        throw new Exception('Failed to initialize');

    $post_data = array(
        'client_id' => $clientId,
        'client_secret' => $clientSecret,
        'grant_type' => 'authorization_code',
        'code' => $code,
        'redirect_uri' => $redirectUri
    );

    curl_setopt($ch, CURLOPT_URL, "https://accounts.spotify.com/api/token");
    curl_setopt($ch, CURLOPT_POST, 1);
    curl_setopt($ch, CURLOPT_POSTFIELDS, http_build_query($post_data));
    curl_setopt($ch, CURLOPT_FOLLOWLOCATION, true);
    curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);

    $response = curl_exec($ch);

    if (curl_errno($ch)) {
        echo 'CURL Error: ' . curl_error($ch);
    }

    curl_close($ch);

    return $response;
}


/*
 * Refresh a token
 * Creates a request to Spotify, which returns a new access_token, refresh_token and token_expiry object
 */
function refreshToken($refresh_token, $clientId, $clientSecret)
{

    $ch = curl_init();

    $post_data = array(
        'client_id' => $clientId,
        'client_secret' => $clientSecret,
        'grant_type' => 'refresh_token',
        'refresh_token' => $refresh_token
    );

    curl_setopt($ch, CURLOPT_URL, "https://accounts.spotify.com/api/token");
    curl_setopt($ch, CURLOPT_POST, 1);
    curl_setopt($ch, CURLOPT_POSTFIELDS, http_build_query($post_data));
    curl_setopt($ch, CURLOPT_FOLLOWLOCATION, true);
    curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);

    $exec = curl_exec($ch);
    $info = curl_getinfo($ch);
    $response = array(
        'exec' => $exec,
        'response_code' => $info['http_code']
    );
    curl_close($ch);

    return $response;
}
