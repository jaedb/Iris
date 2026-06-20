FROM php:8.3-apache
RUN apt-get update && apt-get install -y --no-install-recommends libcurl4-openssl-dev \
    && docker-php-ext-install curl \
    && rm -rf /var/lib/apt/lists/*
COPY auth_spotify.php /var/www/html/auth_spotify.php
