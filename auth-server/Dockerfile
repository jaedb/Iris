FROM php:8.3-apache
RUN apt-get update && apt-get install -y --no-install-recommends libcurl4-openssl-dev \
    && docker-php-ext-install curl \
    && rm -rf /var/lib/apt/lists/* \
    && a2enmod ssl \
    && a2ensite default-ssl
COPY spotify-auth-ssl.conf /etc/apache2/sites-available/spotify-auth-ssl.conf
RUN a2dissite default-ssl && a2ensite spotify-auth-ssl
COPY auth_spotify.php /var/www/html/auth_spotify.php
EXPOSE 443
