# A Linux machine to test Remote SSH against: sshd with key authentication only, git, and
# no JDK. The entrypoint authorizes the public key given in AUTHORIZED_KEY for user `dev`.
FROM ubuntu:24.04
RUN apt-get update \
    && apt-get install -y --no-install-recommends openssh-server git netcat-openbsd procps ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /run/sshd \
    && useradd -m -s /bin/bash dev \
    && usermod -p '*' dev \
    && printf 'PasswordAuthentication no\nKbdInteractiveAuthentication no\nPermitRootLogin no\n' \
        > /etc/ssh/sshd_config.d/test.conf
COPY entrypoint.sh /entrypoint.sh
EXPOSE 22
ENTRYPOINT ["/entrypoint.sh"]
