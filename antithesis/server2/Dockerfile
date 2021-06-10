FROM alpine
COPY --from=nats:2.2.6-alpine3.13 /usr/local/bin/nats-server /nats-server
COPY server2.conf /server2.conf
EXPOSE 4222 5000
ENTRYPOINT ["/nats-server"]
CMD ["--config", "server2.conf", "-addr", "10.20.20.3", "-js", "-sd", "storage"]
