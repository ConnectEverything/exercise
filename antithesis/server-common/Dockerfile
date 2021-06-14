FROM alpine
COPY --from=nats:2.2.6-alpine3.13 /usr/local/bin/nats-server /nats-server
COPY ./server1.conf /server1.conf
COPY ./server2.conf /server2.conf
COPY ./server3.conf /server3.conf
EXPOSE 4222 5000
