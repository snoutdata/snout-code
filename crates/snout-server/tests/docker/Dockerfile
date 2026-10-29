# A Linux machine to test Remote SSH against: sshd with key authentication only, git, no JDK,
# and a small Gradle Java project (a main class and JUnit tests) in /home/dev/demo, a Git
# repository. The entrypoint authorizes the public key given in AUTHORIZED_KEY for user `dev`.
FROM ubuntu:24.04
RUN apt-get update \
    && apt-get install -y --no-install-recommends openssh-server git netcat-openbsd procps ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /run/sshd \
    && useradd -m -s /bin/bash dev \
    && usermod -p '*' dev \
    && printf 'PasswordAuthentication no\nKbdInteractiveAuthentication no\nPermitRootLogin no\n' \
        > /etc/ssh/sshd_config.d/test.conf
COPY --chown=dev:dev demo /home/dev/demo
RUN su dev -c 'cd /home/dev/demo && git init -q -b main && git add -A \
    && git -c user.name=Demo -c user.email=demo@example.com commit -qm "A demo project"'
COPY entrypoint.sh /entrypoint.sh
EXPOSE 22
ENTRYPOINT ["/entrypoint.sh"]
