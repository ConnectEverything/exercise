FROM scratch
ADD server1.conf /
RUN ./nats-server -addr 10.20.20.1 -c ./server1.conf -js -sd storage
